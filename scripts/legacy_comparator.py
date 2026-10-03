#!/usr/bin/env python3
"""Build once, register, and verify the frozen native comparator; never refresh it."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "legacy/baseline.json"
REPOSITORY = "johannes-schmieder/fereg"
PLATFORMS = ("macos", "linux", "windows")
SOURCE_PATHS = (
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "clippy.toml",
    "crates", "legacy", "vendor", "plugin", "scripts", "tests/mock_spi",
    "stata", "ci", "references.lock.toml",
)


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def baseline() -> dict:
    value = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if value["schema"] != 1 or len(value["source_commit"]) != 40:
        raise ValueError("invalid frozen comparator manifest")
    return value


def names(platform: str) -> tuple[str, str]:
    if platform not in PLATFORMS:
        raise ValueError("unsupported comparator platform")
    name = f"fereg_legacy_{platform}.plugin"
    return name, f"{name}.json"


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def verify_source(source: Path, manifest: dict) -> None:
    for name, expected in manifest["source_files"].items():
        if digest(source / name) != expected:
            raise ValueError(f"frozen source mismatch: {name}")


def materialize(source: Path, manifest: dict) -> None:
    revision = manifest["source_commit"]
    actual = git("rev-parse", f"refs/tags/{manifest['tag']}^{{commit}}")
    if actual != revision:
        raise ValueError("comparator tag does not identify the frozen commit")
    for path, expected in manifest["source_trees"].items():
        if git("rev-parse", f"{revision}:{path}") != expected:
            raise ValueError(f"frozen source tree mismatch: {path}")
    source.mkdir(parents=True)
    archive = source.parent / "source.tar"
    subprocess.run(
        ["git", "archive", "--format=tar", "-o", str(archive), revision, *SOURCE_PATHS],
        cwd=ROOT, check=True,
    )
    with tarfile.open(archive) as stream:
        for member in stream.getmembers():
            target = (source / member.name).resolve()
            if source.resolve() not in target.parents or not (member.isfile() or member.isdir()):
                raise ValueError("unexpected path or link in frozen source archive")
        stream.extractall(source)
    archive.unlink()
    verify_source(source, manifest)


def check_candidate(directory: Path, platform: str, manifest: dict) -> dict:
    name, metadata_name = names(platform)
    metadata = json.loads((directory / metadata_name).read_text(encoding="utf-8"))
    for field in ("source_commit", "build_date", "rust_toolchain", "source_files", "source_trees"):
        if metadata.get(field) != manifest[field]:
            raise ValueError(f"comparator provenance mismatch: {field}")
    if metadata.get("schema") != 1 or metadata.get("platform") != platform:
        raise ValueError("comparator metadata has the wrong schema/platform")
    if metadata.get("filename") != name or metadata.get("sha256") != digest(directory / name):
        raise ValueError("comparator binary digest/name mismatch")
    if metadata.get("bytes") != (directory / name).stat().st_size or metadata["bytes"] == 0:
        raise ValueError("comparator binary size mismatch")
    fields = (directory / f"{name}.sha256").read_text(encoding="ascii").split()
    if fields != [metadata["sha256"], name]:
        raise ValueError("comparator checksum file mismatch")
    return metadata


def verify(directory: Path, platform: str, manifest: dict | None = None) -> dict:
    manifest = baseline() if manifest is None else manifest
    metadata = check_candidate(directory, platform, manifest)
    expected = manifest["assets"].get(platform)
    if expected is None:
        raise ValueError(f"no canonical {platform} artifact is registered; do not rebuild as a substitute")
    name, metadata_name = names(platform)
    if expected != {
        "filename": name, "sha256": metadata["sha256"],
        "metadata_sha256": digest(directory / metadata_name),
    }:
        raise ValueError("artifact differs from the registered canonical comparator")
    return metadata


def register(directory: Path, platform: str) -> None:
    manifest = baseline()
    metadata = check_candidate(directory, platform, manifest)
    name, metadata_name = names(platform)
    record = {"filename": name, "sha256": metadata["sha256"],
              "metadata_sha256": digest(directory / metadata_name)}
    previous = manifest["assets"].get(platform)
    if previous is not None and previous != record:
        raise ValueError("refusing to replace an already registered comparator")
    manifest["assets"][platform] = record
    MANIFEST.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Registered canonical {platform}: {record['sha256']}")


def build(directory: Path, platform: str) -> None:
    manifest = baseline()
    if directory.exists():
        raise ValueError("build output must be new; never overwrite a comparator")
    version = subprocess.check_output(["rustc", "--version"], text=True).strip()
    if version.split()[1] != manifest["rust_toolchain"]:
        raise ValueError(f"wrong comparator toolchain: {version}")
    build_root = ROOT / "target/legacy-comparator"
    build_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f"{platform}-", dir=build_root) as temporary:
        source = Path(temporary) / "source"
        materialize(source, manifest)
        environment = os.environ.copy()
        environment.update(FEREG_GIT_COMMIT=manifest["source_commit"][:12],
                           FEREG_BUILD_DATE=manifest["build_date"], RUSTFLAGS="")
        environment.pop("CARGO_ENCODED_RUSTFLAGS", None)
        # Cache compilation, but isolate it from the active next build.
        environment["CARGO_TARGET_DIR"] = str(build_root / f"cargo-{platform}")
        if platform == "windows":
            environment.pop("CARGO_TARGET_DIR", None)  # The pinned PS1 uses source/target.
            command = ["powershell", "-NoProfile", "-File", str(source / "scripts/build_windows.ps1"),
                       "-SourceRoot", str(source), "-ExpectedSourceCommit", manifest["source_commit"],
                       "-ExpectedBuildDate", manifest["build_date"]]
        else:
            command = ["bash", str(source / f"scripts/build_{platform}.sh")]
        subprocess.run(command, cwd=source, env=environment, check=True)
        verify_source(source, manifest)
        original = source / "dist" / f"fereg_{platform}.plugin"
        directory.mkdir(parents=True)
        name, metadata_name = names(platform)
        shutil.copyfile(original, directory / name)
        metadata = {key: manifest[key] for key in
                    ("source_commit", "build_date", "rust_toolchain", "source_files", "source_trees")}
        metadata.update(schema=1, platform=platform, filename=name, sha256=digest(directory / name),
                        bytes=original.stat().st_size, rustc=version,
                        checks=["pinned-native-build", "binary-audit", "existing-actual-plugin-mock"])
        (directory / metadata_name).write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
        (directory / f"{name}.sha256").write_text(f"{metadata['sha256']}  {name}\n", encoding="ascii")
        check_candidate(directory, platform, manifest)
        print(f"Built comparator candidate {name}: {metadata['sha256']}; registration is separate")


def fetch(directory: Path, platform: str) -> None:
    manifest = baseline()
    if platform not in manifest["assets"]:
        raise ValueError("canonical artifact not registered; fetch never builds")
    name, metadata_name = names(platform)
    if (directory / name).exists():
        verify(directory, platform, manifest)
        return
    directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fereg-comparator-") as temporary:
        subprocess.run(["gh", "release", "download", manifest["tag"], "--repo", REPOSITORY,
                        "--dir", temporary, "--pattern", name, "--pattern", f"{name}.sha256",
                        "--pattern", metadata_name], check=True)
        verify(Path(temporary), platform, manifest)
        for filename in (name, f"{name}.sha256", metadata_name):
            target = directory / filename
            if target.exists():
                raise ValueError(f"refusing to overwrite a partial comparator cache: {filename}")
        for filename in (name, f"{name}.sha256", metadata_name):
            shutil.copyfile(Path(temporary) / filename, directory / filename)
    verify(directory, platform, manifest)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "register", "verify", "fetch"))
    parser.add_argument("--platform", choices=PLATFORMS, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    functions = {"build": build, "register": register, "verify": verify, "fetch": fetch}
    functions[args.action](args.directory.resolve(), args.platform)


if __name__ == "__main__":
    main()
