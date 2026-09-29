"""Build arm64 local prototypes with the same release settings and ABI checks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def build(root, names):
    root = root.resolve()
    env = dict(os.environ, DEVELOPER_DIR=os.environ.get("DEVELOPER_DIR", "/Library/Developer/CommandLineTools"),
               CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(root / "build"),
               FEREG_GIT_COMMIT="55c19cb6", FEREG_BUILD_DATE="2026-09-19")
    for name in names:
        source = root / name
        (source / "target/native").mkdir(parents=True, exist_ok=True)
        with (root / ("build-" + name + ".log")).open("w") as log:
            def run(command):
                subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
            run(["cargo", "fmt", "--all"])
            run(["cargo", "build", "--locked", "--offline", "--release", "-p", "fereg-plugin", "--target", "aarch64-apple-darwin"])
            for unit in ("stplugin", "stata_bridge"):
                run(["clang", "-std=c11", "-O3", "-fPIC", "-Wall", "-Wextra", "-Werror", "-DSYSTEM=APPLEMAC", "-target", "arm64-apple-macos11", "-mmacosx-version-min=11.0", "-Iplugin", "-c", f"plugin/{unit}.c", "-o", f"target/native/{unit}.o"])
            archive = root / "build/aarch64-apple-darwin/release/libfereg_plugin.a"
            run(["clang", "-bundle", "-target", "arm64-apple-macos11", "-mmacosx-version-min=11.0", "-Wl,-dead_strip", "-Wl,-exported_symbols_list,plugin/exports_macos.txt", "target/native/stplugin.o", "target/native/stata_bridge.o", str(archive), "-framework", "Security", "-framework", "CoreFoundation", "-framework", "SystemConfiguration", "-liconv", "-lresolv", "-o", "dist/fereg_macos.plugin"])
            run(["codesign", "--force", "--sign", "-", "dist/fereg_macos.plugin"])
            run(["bash", "tests/mock_spi/run_actual_unix_plugin_test.sh", "dist/fereg_macos.plugin"])
        sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
        (root / ("identity-" + name + ".json")).write_text(json.dumps(dict(plugin=sha(source / "dist/fereg_macos.plugin"), rust_archive=sha(archive), changed_source={str(p.relative_to(source)): sha(p) for p in [source / "legacy/fereg-core/src/ols/prepared_fit.rs", source / "legacy/fereg-core/src/ols/prepared_rhs.rs"]}, abi="pass"), indent=2)+"\n")
        print("BUILD_PASS", name, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("names", nargs="+")
    a = p.parse_args()
    build(a.root, a.names)
