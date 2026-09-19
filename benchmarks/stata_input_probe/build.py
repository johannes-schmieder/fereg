"""Relink isolated accessor variants against an unchanged, previously built Rust archive."""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

REPO = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(root, previous):
    root = root.resolve()
    previous = previous.resolve()
    root.mkdir(parents=True, exist_ok=True)
    original = (previous / "baseline/plugin/stata_bridge.c").read_text()
    start = original.index("ST_int fereg_spi_vdata_columns(")
    end = original.index("\nST_int fereg_spi_vstore(", start)
    function = original[start:end]
    guard = '''    /* Diagnostic: establish the numeric batch's bounds once. */
    if (first_observation > SF_nobs() ||
        observation_count > SF_nobs() - first_observation + 1) return 498;
    for (variable = 0; variable < variable_count; ++variable) {
        if (columns[variable] == NULL) return FEREG_SPI_RC_SYNTAX;
        if (variables[variable] <= 0 || variables[variable] > SF_nvars() ||
            SF_var_is_string(variables[variable])) return 498;
    }
    reader = _stata_->ACCESSOR;
    if (reader == NULL) return FEREG_SPI_RC_UNAVAILABLE;
'''
    env = os.environ.copy()
    env["DEVELOPER_DIR"] = "/Library/Developer/CommandLineTools"
    identities = {}
    archive = previous / "build-baseline/aarch64-apple-darwin/release/libfereg_plugin.a"
    for mode in ("baseline", "cached", "fast"):
        source = root / mode
        source.mkdir(exist_ok=False)
        for folder in ("plugin", "stata", "tests/mock_spi"):
            shutil.copytree(previous / "baseline" / folder, source / folder)
        (source / "dist").mkdir()
        (source / "build").mkdir()
        text = original
        if mode != "baseline":
            altered = function.replace("    ST_int rc;", "    ST_int rc;\n    ST_IIIDp reader;")
            altered = altered.replace("    *missing_value = SV_missval;", guard.replace("ACCESSOR", "vdata" if mode == "fast" else "safevdata") + "    *missing_value = SV_missval;")
            altered = altered.replace("            if (columns[variable] == NULL) return FEREG_SPI_RC_SYNTAX;\n", "")
            altered = altered.replace("rc = SF_vdata(", "rc = reader(")
            text = original[:start] + altered + original[end:]
        (source / "plugin/stata_bridge.c").write_text(text)
        (root / (mode + ".patch")).write_text("".join(difflib.unified_diff(original.splitlines(True), text.splitlines(True), fromfile="a/plugin/stata_bridge.c", tofile="b/plugin/stata_bridge.c")))
        # The SDK has both callbacks. Existing mocks initialized only safevdata.
        test = source / "tests/mock_spi/actual_plugin_loader_test.c"
        test.write_text(test.read_text().replace("    plugin.safevdata = mock_vdata;", "    plugin.safevdata = mock_vdata;\n    plugin.vdata = mock_vdata;"))
        with (source / "build.log").open("w") as log:
            def run(args):
                subprocess.run(args, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
            for name in ("stplugin", "stata_bridge"):
                run(["/usr/bin/clang", "-std=c11", "-O3", "-fPIC", "-Wall", "-Wextra", "-Wpedantic", "-Wconversion", "-Wshadow", "-Wstrict-prototypes", "-Wmissing-prototypes", "-Werror", "-DSYSTEM=APPLEMAC", "-target", "arm64-apple-macos11", "-mmacosx-version-min=11.0", "-Iplugin", "-c", f"plugin/{name}.c", "-o", f"build/{name}.o"])
            run(["/usr/bin/clang", "-bundle", "-target", "arm64-apple-macos11", "-mmacosx-version-min=11.0", "-Wl,-dead_strip", "-Wl,-exported_symbols_list,plugin/exports_macos.txt", "build/stplugin.o", "build/stata_bridge.o", str(archive), "-framework", "Security", "-framework", "CoreFoundation", "-framework", "SystemConfiguration", "-liconv", "-lresolv", "-o", "dist/fereg_macos.plugin"])
            run(["/usr/bin/codesign", "--force", "--sign", "-", "dist/fereg_macos.plugin"])
            run(["bash", "tests/mock_spi/run_actual_unix_plugin_test.sh", "dist/fereg_macos.plugin"])
            if mode != "baseline":
                run(["/usr/bin/clang", "-std=c11", "-O2", "-g", "-fsanitize=undefined", "-fno-sanitize-recover=all", "-Wall", "-Wextra", "-Werror", "-DSYSTEM=APPLEMAC", "-Iplugin", "plugin/stplugin.c", "plugin/stata_bridge.c", str(REPO / "benchmarks/stata_input_probe/guard_test.c"), "-o", "build/guard_test_ubsan"])
                with (source / "guard-ubsan.log").open("w") as guard_log:
                    subprocess.run([str(source / "build/guard_test_ubsan")], env=env, stdout=guard_log, stderr=subprocess.STDOUT, timeout=15, check=True)
        identities[mode] = {"plugin": sha(source / "dist/fereg_macos.plugin"), "bridge": sha(source / "plugin/stata_bridge.c"), "patch": sha(root / (mode + ".patch")), "abi_test": "pass"}
        print(mode, identities[mode], flush=True)
    identities["rust_archive_sha256"] = sha(archive)
    identities["source_commit"] = (previous / "source-commit.txt").read_text().strip()
    (root / "build-identities.json").write_text(json.dumps(identities, indent=2) + "\n")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("previous", type=Path)
    a = p.parse_args()
    build(a.root, a.previous)
