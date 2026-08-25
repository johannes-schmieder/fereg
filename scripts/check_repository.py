#!/usr/bin/env python3
"""Static fereg 0.9.0 source-candidate checks requiring neither Rust nor Stata."""
from __future__ import annotations

import ast
import hashlib
import json
import re
import subprocess
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


# Historical receipts and captured evidence are immutable and may retain the
# former package identity. Active source, build, package, CI, and test files may
# not reintroduce it or depend on the removed development-automation package.
legacy_identifiers = re.compile(
    r"(?:St" r"Fixest|ST" r"FIXEST|st" r"fixest|ST" r"FEOLS|st" r"feols|ST" r"FEPOIS|st" r"fepois)"
)
preserved_evidence_prefixes = (
    ".ci/stata/results/",
    ".ci/stata/full-results/",
    ".ci/reference-probes/",
    ".ci/staged-validation/",
    "build_evidence/",
    "benchmarks/reports/captured/",
)
active_text_suffixes = {
    ".ado",
    ".c",
    ".do",
    ".h",
    ".json",
    ".lock",
    ".pkg",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".sthlp",
    ".toc",
    ".toml",
    ".yaml",
    ".yml",
}
tracked_files = subprocess.run(
    ["git", "-C", str(ROOT), "ls-files", "-z"],
    check=True,
    stdout=subprocess.PIPE,
).stdout.split(b"\0")
for raw_relative in tracked_files:
    if not raw_relative:
        continue
    relative = raw_relative.decode("utf-8")
    path = ROOT / relative
    if not path.is_file():
        continue
    if relative == ".ci/stata/latest.json" or relative.startswith(
        preserved_evidence_prefixes
    ):
        continue
    if path.suffix.lower() not in active_text_suffixes:
        continue
    require(
        legacy_identifiers.search(relative) is None,
        f"active path retains legacy package identity: {relative}",
    )
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        continue
    require(
        legacy_identifiers.search(text) is None,
        f"active file retains legacy package identity: {relative}",
    )
    require(
        "scripts" + ".development" not in text,
        f"active file depends on removed development automation: {relative}",
    )


required_files = [
    "AGENTS.md",
    "STATUS.md",
    "Cargo.toml",
    "Cargo.lock",
    "references.lock.toml",
    "benchmarks/manifests/ols_v1.json",
    "benchmarks/manifests/ols_hidden_numeric_candidate_receipt_v1.json",
    "benchmarks/stata/baseline_reghdfe.do",
    "scripts/ols_reference.py",
    "scripts/stage_release.py",
    "scripts/package_release.py",
    "scripts/run_dependency_audit.py",
    "docs/adr/0007-fereg-090-source-qualification-contract.md",
    "crates/fereg-core/src/lib.rs",
    "crates/fereg-plugin/src/ffi.rs",
    "plugin/stplugin.c",
    "plugin/stplugin.h",
    "plugin/stata_bridge.c",
    "plugin/stata_bridge.h",
    "stata/fereg.ado",
    "stata/fereg_parse.ado",
    "scripts/build_linux.sh",
    "scripts/build_macos.sh",
    "scripts/build_windows.ps1",
]
for item in required_files:
    read(item)

require(
    not (ROOT / "ci/source-transforms/apply_option_a_absorbed_slope.py").exists(),
    "the applied Option A migration script must remain deleted",
)
require(
    not (ROOT / "scripts/development").exists(),
    "deleted development automation must not be restored",
)

status_files = [
    path.relative_to(ROOT).as_posix()
    for path in ROOT.rglob("*.md")
    if "status" in path.name.lower()
    or path.name
    in {
        "HANDOFF.md",
        "IMPLEMENTATION_STATUS.md",
        "LOCAL_CODEX_PROMPT.md",
        "VALIDATION.md",
    }
]
require(
    sorted(status_files) == ["STATUS.md"],
    "STATUS.md must be the only mutable project-status document; found: "
    + ", ".join(sorted(status_files)),
)
status = read("STATUS.md")
for heading in (
    "## Milestone exit criteria",
    "## Completed, evidence-backed baseline",
    "## Active checkpoint",
    "## Current blockers",
    "## Next evidence to record",
):
    require(heading in status, f"STATUS.md is missing required section: {heading}")

agents = read("AGENTS.md")
require("# fereg 1.0 Rust-First Implementation Plan for Codex" in agents, "AGENTS.md is not the supplied specification")
require(
    "0.9.0" in read("docs/adr/0007-fereg-090-source-qualification-contract.md"),
    "ADR-0007 release-candidate contract is missing",
)

root_cargo = read("Cargo.toml")
for setting in ['lto = "fat"', "codegen-units = 1", 'panic = "unwind"', "strip = true"]:
    require(setting in root_cargo, f"release profile missing {setting}")
require("target-cpu=native" not in root_cargo, "distributed build uses target-cpu=native")

core_files = list((ROOT / "crates/fereg-core/src").glob("*.rs"))
require("#![forbid(unsafe_code)]" in read("crates/fereg-core/src/lib.rs"), "safe core does not forbid unsafe code")
for path in core_files:
    text = path.read_text(encoding="utf-8")
    require(not re.search(r"\bunsafe\b", text), f"unsafe token found in safe core: {path.relative_to(ROOT)}")

plugin_rs = list((ROOT / "crates/fereg-plugin/src").glob("*.rs"))
for path in plugin_rs:
    text = path.read_text(encoding="utf-8")
    if re.search(r"\bunsafe\b", text):
        require(path.name == "ffi.rs", f"unsafe code escaped ffi.rs: {path.relative_to(ROOT)}")
ffi = read("crates/fereg-plugin/src/ffi.rs")
require("catch_unwind" in ffi, "FFI boundary lacks catch_unwind")
require("#[no_mangle]" in ffi and "fereg_dispatch" in ffi, "exported Rust dispatch missing")

bridge = read("plugin/stata_bridge.c")
require(re.search(r"STDLL\s+stata_call\s*\(", bridge) is not None, "C bridge lacks stata_call")
require("fereg_dispatch" in bridge, "C bridge does not dispatch to Rust")
for forbidden in ("estimate-ols", "estimate-ppml", "irls", "singleton"):
    require(forbidden not in bridge.lower(), f"scientific action {forbidden} appears in C bridge")

parse_ado = read("stata/fereg_parse.ado")
code_lines = [line for line in parse_ado.splitlines() if not line.lstrip().startswith(("*", "/*"))]
parse_code = "\n".join(code_lines).lower()
for forbidden in ("fvrevar ", "egen group", "tabulate, generate", "reghdfe", "ppmlhdfe", "ftools"):
    require(forbidden not in parse_code, f"metadata parser contains forbidden materialization/runtime command: {forbidden}")
require("touse(varname numeric)" in parse_code, "parser does not require a caller-owned candidate marker")
require("marksample touse" not in parse_code, "parser creates a short-lived candidate marker")
require("fvexpand `rhs' if `touse'" in parse_code, "factor expansion does not use the caller-owned candidate marker")
require("fvexpand" in parse_code and "_ms_parse_parts" in parse_code, "parser does not use Stata factor metadata machinery")

fereg_ado = read("stata/fereg.ado").lower()
require(
    "_fereg_estimate_public `0'" in fereg_ado,
    "fereg.ado does not delegate to the guarded public OLS transaction",
)
require(
    "plugin call" not in fereg_ado,
    "fereg.ado bypasses the single guarded substantive plugin boundary",
)

feppml_ado = read("stata/feppml.ado").lower()
require("exit 498" in feppml_ado, "feppml.ado does not fail closed")
require(
    "plugin call" not in feppml_ado,
    "feppml.ado makes a substantive plugin call before PPML implementation",
)

loader = read("stata/_fereg_load.ado")
for plugin in ("fereg_windows.plugin", "fereg_linux.plugin", "fereg_macos.plugin"):
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
for name in ("pyfixest", "within", "reghdfe", "ftools", "require", "ppmlhdfe", "stata-spi"):
    require(name in refs, f"reference lock missing {name}")
    if name in refs:
        require(len(refs[name]["commit"]) >= 12, f"reference {name} is not commit-pinned")
spi = refs.get("stata-spi", {})
if spi:
    require(spi.get("local_stplugin_c_sha256") == sha256(ROOT / "plugin/stplugin.c"), "stplugin.c hash mismatch")
    require(spi.get("local_stplugin_h_sha256") == sha256(ROOT / "plugin/stplugin.h"), "stplugin.h hash mismatch")

expected_capture_pins = {
    "reghdfe": "29b2b203f534413369d675d0c0f674d61291f536",
    "ftools": "d6e24d66fd02ba1b37325c82c5040444c5b4c636",
    "require": "f4760b86e55cf73b82041ce59cf20e8e8224afb0",
}
for name, commit in expected_capture_pins.items():
    entry = refs.get(name, {})
    require(entry.get("pin_status") == "resolved" and entry.get("commit") == commit, f"{name} exact capture pin is stale")
    files = entry.get("required_capture_files", [])
    hashes = entry.get("required_capture_sha256", {})
    require(set(files) == set(hashes) and all(re.fullmatch(r"[0-9a-f]{64}", value) for value in hashes.values()), f"{name} capture file hashes are not closed")
require("src/lftools.mlib" not in refs.get("ftools", {}).get("required_capture_files", []), "generated lftools.mlib must not be a locked source input")

try:
    ols_manifest = json.loads(read("benchmarks/manifests/ols_v1.json"))
except json.JSONDecodeError as error:
    ERRORS.append(f"OLS v1 manifest is invalid JSON: {error}")
    ols_manifest = {}
require(ols_manifest.get("manifest_id") == "ols_v1", "OLS v1 manifest identity missing")
require(ols_manifest.get("frozen") is True, "OLS v1 manifest is not frozen")
benchmark_contract = ols_manifest.get("benchmark_contract", {})
require(
    benchmark_contract.get("candidate_state") == "enabled",
    "OLS benchmark candidate is not enabled",
)
require(
    benchmark_contract.get("thresholds", {}).get(
        "geometric_mean_time_ratio_max"
    )
    == 0.4,
    "OLS benchmark does not encode the governing 2.5x geometric-mean gate",
)

try:
    candidate_receipt_identity = json.loads(
        read("benchmarks/manifests/ols_hidden_numeric_candidate_receipt_v1.json")
    )
except json.JSONDecodeError as error:
    ERRORS.append(f"hidden numeric candidate receipt identity is invalid JSON: {error}")
    candidate_receipt_identity = {}
require(
    set(candidate_receipt_identity)
    == {
        "schema_version",
        "identity",
        "receipt_path",
        "git_object_format",
        "git_blob",
        "git_mode",
        "file_sha256",
        "payload_sha256",
    },
    "hidden numeric candidate receipt identity schema is not closed",
)
candidate_receipt_relative = candidate_receipt_identity.get("receipt_path", "")
candidate_receipt = ROOT / candidate_receipt_relative
require(
    candidate_receipt_identity.get("schema_version") == 1
    and candidate_receipt_identity.get("identity")
    == "fereg-ols-hidden-numeric-v1-canonical-receipt",
    "hidden numeric canonical receipt identity is stale",
)
require(
    bool(candidate_receipt_relative)
    and not Path(candidate_receipt_relative).is_absolute()
    and ".." not in Path(candidate_receipt_relative).parts
    and candidate_receipt.is_file(),
    "hidden numeric canonical receipt path is invalid",
)
if candidate_receipt.is_file():
    require(
        candidate_receipt_identity.get("file_sha256") == sha256(candidate_receipt),
        "hidden numeric canonical receipt file SHA is stale",
    )
    try:
        candidate_document = json.loads(candidate_receipt.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        ERRORS.append(f"hidden numeric canonical receipt is invalid JSON: {error}")
        candidate_document = {}
    require(
        candidate_document.get("seal", {}).get("payload_sha256")
        == candidate_receipt_identity.get("payload_sha256"),
        "hidden numeric canonical receipt payload seal is stale",
    )
    observed_blob = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "--verify", f"HEAD:{candidate_receipt_relative}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    ).stdout.strip()
    observed_tree = subprocess.run(
        ["git", "-C", str(ROOT), "ls-tree", "HEAD", "--", candidate_receipt_relative],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    ).stdout.split()
    require(
        candidate_receipt_identity.get("git_object_format")
        == subprocess.run(
            ["git", "-C", str(ROOT), "rev-parse", "--show-object-format"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        ).stdout.strip(),
        "hidden numeric canonical receipt Git object format is stale",
    )
    require(
        observed_blob == candidate_receipt_identity.get("git_blob")
        and len(observed_tree) >= 3
        and observed_tree[0] == candidate_receipt_identity.get("git_mode") == "100644"
        and observed_tree[1] == "blob",
        "hidden numeric canonical receipt Git blob/mode is stale",
    )
capture = ols_manifest.get("capture", {})
for name in ("script", "runner", "generator"):
    spec = capture.get(name, {})
    relative = spec.get("path", "")
    path = ROOT / relative
    require(path.is_file(), f"OLS capture {name} is missing: {relative}")
    if path.is_file():
        require(spec.get("sha256") == sha256(path), f"OLS capture {name} hash mismatch")

for manifest in ROOT.glob("crates/*/Cargo.toml"):
    text = manifest.read_text(encoding="utf-8").lower()
    for forbidden in ("openblas", "mkl", "openmp", "pyo3", "numpy", "julia"):
        require(forbidden not in text, f"forbidden runtime dependency in {manifest.relative_to(ROOT)}: {forbidden}")

# Parse Python with its real grammar; use a cheap delimiter check only for Rust
# when Cargo is unavailable.
for path in ROOT.glob("scripts/*.py"):
    try:
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except SyntaxError as error:
        ERRORS.append(f"Python syntax error in {path.relative_to(ROOT)}: {error}")

for path in ROOT.glob("crates/**/*.rs"):
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
