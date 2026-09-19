"""Isolated, source-hashed arm64 builds for the one-FE development campaign."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(root, name, revision, overlays, profile_preparation=False):
    root = root.resolve()
    source = root / name
    source.mkdir(parents=True, exist_ok=False)
    commit = subprocess.check_output(["/opt/homebrew/bin/git", "rev-parse", revision], cwd=REPO, text=True).strip()
    archive = subprocess.check_output(["/opt/homebrew/bin/git", "archive", commit,
        "Cargo.toml", "Cargo.lock", "crates", "plugin", "stata", "tests", "scripts", "references.lock.toml"], cwd=REPO)
    with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
        bundle.extractall(source, filter="data")
    for relative in overlays:
        path = source / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((REPO / relative).read_bytes())
    if profile_preparation:
        from prepare_profile import instrument
        instrument(source)
    hashes = {str(p.relative_to(source)): sha(p) for p in sorted(source.rglob("*")) if p.is_file()}
    target = root / ("cargo-" + name)
    env = dict(os.environ, DEVELOPER_DIR="/Library/Developer/CommandLineTools",
               CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(target),
               FEREG_GIT_COMMIT=commit[:8], FEREG_BUILD_DATE="2026-09-19")
    (source / "target/native").mkdir(parents=True)
    (source / "dist").mkdir()
    with (root / ("build-" + name + ".log")).open("w") as log:
        def run(command):
            subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        run(["cargo", "build", "--locked", "--offline", "--release", "-p", "fereg-plugin", "--target", "aarch64-apple-darwin"])
        for unit in ("stplugin", "stata_bridge"):
            run(["clang", "-std=c11", "-O3", "-fPIC", "-Wall", "-Wextra", "-Werror", "-DSYSTEM=APPLEMAC",
                 "-target", "arm64-apple-macos11", "-Iplugin", "-c", f"plugin/{unit}.c", "-o", f"target/native/{unit}.o"])
        library = target / "aarch64-apple-darwin/release/libfereg_plugin.a"
        run(["clang", "-bundle", "-target", "arm64-apple-macos11", "-Wl,-dead_strip",
             "-Wl,-exported_symbols_list,plugin/exports_macos.txt", "target/native/stplugin.o", "target/native/stata_bridge.o",
             str(library), "-framework", "Security", "-framework", "CoreFoundation", "-framework", "SystemConfiguration",
             "-liconv", "-lresolv", "-o", "dist/fereg_macos.plugin"])
        run(["codesign", "--force", "--sign", "-", "dist/fereg_macos.plugin"])
        run(["bash", "tests/mock_spi/run_actual_unix_plugin_test.sh", "dist/fereg_macos.plugin"])
    identity = dict(base_commit=commit, overlays=overlays, source=hashes, plugin=sha(source / "dist/fereg_macos.plugin"),
                    rust_archive=sha(library), abi="pass", target="aarch64-apple-darwin")
    if profile_preparation:
        identity["diagnostics"] = ["preparation_phases"]
    (root / ("identity-" + name + ".json")).write_text(json.dumps(identity, indent=2) + "\n")
    print("BUILD_PASS", name, identity["plugin"], flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("name")
    p.add_argument("--revision", default="HEAD")
    p.add_argument("--overlay", nargs="*", default=[])
    p.add_argument("--profile-preparation", action="store_true")
    a = p.parse_args()
    build(a.root, a.name, a.revision, a.overlay, a.profile_preparation)
