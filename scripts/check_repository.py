#!/usr/bin/env python3
"""Static Phase 0-1 contract checks that require neither Rust nor Stata."""
from __future__ import annotations

import hashlib
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ERRORS: list[str] = []


def require(condition: bool, message: str) -> None:
    if not condition:
        ERRORS.append(message)


def read(relative: str) -> str:
    path = ROOT / relative
    require(path.is_file(), f"missing required file: {relative}")
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


required_files = [
    "AGENTS.md",
    "Cargo.toml",
    "Cargo.lock",
    "references.lock.toml",
    "crates/stfixest-core/src/lib.rs",
    "crates/stfixest-plugin/src/ffi.rs",
    "plugin/stplugin.c",
    "plugin/stplugin.h",
    "plugin/stata_bridge.c",
    "plugin/stata_bridge.h",
    "stata/stfixest.ado",
    "stata/stfixest_parse.ado",
    "scripts/build_linux.sh",
    "scripts/build_macos.sh",
    "scripts/build_windows.ps1",
]
for item in required_files:
    read(item)

agents = read("AGENTS.md")
require("# StFixest 1.0 Rust-First Implementation Plan for Codex" in agents, "AGENTS.md is not the supplied specification")
require("Begin with **Phase 0 and Phase 1 only**" in agents, "AGENTS.md Phase 0-1 assignment missing")

root_cargo = read("Cargo.toml")
for setting in ['lto = "fat"', "codegen-units = 1", 'panic = "unwind"', "strip = true"]:
    require(setting in root_cargo, f"release profile missing {setting}")
require("target-cpu=native" not in root_cargo, "distributed build uses target-cpu=native")

core_files = list((ROOT / "crates/stfixest-core/src").glob("*.rs"))
require("#![forbid(unsafe_code)]" in read("crates/stfixest-core/src/lib.rs"), "safe core does not forbid unsafe code")
for path in core_files:
    text = path.read_text(encoding="utf-8")
    require(not re.search(r"\bunsafe\b", text), f"unsafe token found in safe core: {path.relative_to(ROOT)}")

plugin_rs = list((ROOT / "crates/stfixest-plugin/src").glob("*.rs"))
for path in plugin_rs:
    text = path.read_text(encoding="utf-8")
    if re.search(r"\bunsafe\b", text):
        require(path.name == "ffi.rs", f"unsafe code escaped ffi.rs: {path.relative_to(ROOT)}")
ffi = read("crates/stfixest-plugin/src/ffi.rs")
require("catch_unwind" in ffi, "FFI boundary lacks catch_unwind")
require("#[no_mangle]" in ffi and "stfixest_dispatch" in ffi, "exported Rust dispatch missing")

bridge = read("plugin/stata_bridge.c")
require(re.search(r"STDLL\s+stata_call\s*\(", bridge) is not None, "C bridge lacks stata_call")
require("stfixest_dispatch" in bridge, "C bridge does not dispatch to Rust")
for forbidden in ("estimate-ols", "estimate-ppml", "irls", "singleton"):
    require(forbidden not in bridge.lower(), f"scientific action {forbidden} appears in C bridge")

parse_ado = read("stata/stfixest_parse.ado")
code_lines = [line for line in parse_ado.splitlines() if not line.lstrip().startswith(("*", "/*"))]
parse_code = "\n".join(code_lines).lower()
for forbidden in ("fvrevar ", "egen group", "tabulate, generate", "reghdfe", "ppmlhdfe", "ftools"):
    require(forbidden not in parse_code, f"metadata parser contains forbidden materialization/runtime command: {forbidden}")
require("touse(varname numeric)" in parse_code, "parser does not require a caller-owned candidate marker")
require("marksample touse" not in parse_code, "parser creates a short-lived candidate marker")
require("fvexpand `rhs' if `touse'" in parse_code, "factor expansion does not use the caller-owned candidate marker")
require("fvexpand" in parse_code and "_ms_parse_parts" in parse_code, "parser does not use Stata factor metadata machinery")

for estimator in ("stfeols.ado", "stfepois.ado"):
    text = read(f"stata/{estimator}").lower()
    require("exit 498" in text, f"{estimator} does not fail closed")
    require("plugin call" not in text, f"{estimator} makes a substantive plugin call before estimator implementation")

loader = read("stata/_stfixest_load.ado")
for plugin in ("stfixest_windows.plugin", "stfixest_linux.plugin", "stfixest_macos.plugin"):
    require(plugin in loader, f"platform loader missing {plugin}")
require("version 18.0" in loader, "Stata 18 version guard missing")

build_linux = read("scripts/build_linux.sh")
build_macos = read("scripts/build_macos.sh")
build_windows = read("scripts/build_windows.ps1")
require("x86_64-unknown-linux-gnu" in build_linux, "Linux target missing")
require("x86_64-apple-darwin" in build_macos and "aarch64-apple-darwin" in build_macos, "macOS slices missing")
require("lipo -create" in build_macos, "macOS universal join missing")
require("x86_64-pc-windows-msvc" in build_windows, "Windows x86-64 target missing")
require("crt-static" in build_windows and "/MT" in build_windows, "Windows static CRT policy missing")

with (ROOT / "references.lock.toml").open("rb") as handle:
    lock = tomllib.load(handle)
spec_lock = lock.get("governing_spec", {})
require(spec_lock.get("file") == "AGENTS.md", "governing specification filename is not locked")
require(
    spec_lock.get("sha256") == sha256(ROOT / "AGENTS.md"),
    "AGENTS.md hash does not match the governing specification lock",
)
refs = {entry["name"]: entry for entry in lock["reference"]}
for name in ("pyfixest", "within", "reghdfe", "ppmlhdfe", "stata-spi"):
    require(name in refs, f"reference lock missing {name}")
    if name in refs:
        require(len(refs[name]["commit"]) >= 12, f"reference {name} is not commit-pinned")
spi = refs.get("stata-spi", {})
if spi:
    require(spi.get("local_stplugin_c_sha256") == sha256(ROOT / "plugin/stplugin.c"), "stplugin.c hash mismatch")
    require(spi.get("local_stplugin_h_sha256") == sha256(ROOT / "plugin/stplugin.h"), "stplugin.h hash mismatch")

for manifest in ROOT.glob("crates/*/Cargo.toml"):
    text = manifest.read_text(encoding="utf-8").lower()
    for forbidden in ("openblas", "mkl", "openmp", "pyo3", "numpy", "julia"):
        require(forbidden not in text, f"forbidden runtime dependency in {manifest.relative_to(ROOT)}: {forbidden}")

# Cheap delimiter check catches common generated-source damage before Cargo is available.
for path in [*ROOT.glob("crates/**/*.rs"), *ROOT.glob("scripts/*.py")]:
    text = path.read_text(encoding="utf-8")
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    in_string = False
    escaped = False
    for char in text:
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue
        if char == '"':
            in_string = True
        elif char in pairs:
            stack.append(pairs[char])
        elif char in pairs.values():
            if not stack or stack.pop() != char:
                ERRORS.append(f"unbalanced delimiter in {path.relative_to(ROOT)}")
                break
    else:
        require(not stack, f"unclosed delimiter in {path.relative_to(ROOT)}")

if ERRORS:
    print("repository checks: FAIL", file=sys.stderr)
    for error in ERRORS:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print("repository checks: PASS")
