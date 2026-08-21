#!/usr/bin/env python3
"""Fail-closed audits for release Stata plugins.

The audit is intentionally target-native: it uses the platform's object tools
to inspect the binary that will be shipped. Pure parsing and validation live in
small functions so all platform policies can be tested on any host.
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

MAX_GLIBC = (2, 28)
REQUIRED_EXPORTS = frozenset({"pginit", "stata_call"})
MACOS_ARCHITECTURES = frozenset({"x86_64", "arm64"})

# Match dependency basenames, not arbitrary symbol text. A function containing
# "python" is not evidence of a dynamic Python dependency.
FORBIDDEN_DEPENDENCY_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("Rust", re.compile(r"(?:^|[-_])(?:lib)?(?:rustc|rust_std|std-[0-9a-f]{8,}|panic_unwind)(?:[-_.]|$)", re.I)),
    ("Python", re.compile(r"(?:^|lib)python(?:\d|\.|$)", re.I)),
    ("Julia", re.compile(r"(?:^|lib)julia(?:\d|\.|$)", re.I)),
    ("BLAS/LAPACK", re.compile(r"(?:openblas|mkl|^lib(?:c?blas|lapack)(?:\.|-)|^(?:c?blas|lapack)\.dll$)", re.I)),
    ("OpenMP", re.compile(r"(?:libgomp|libomp|libiomp|vcomp\d*)", re.I)),
    ("TBB", re.compile(r"(?:^|lib)tbb(?:malloc|malloc_proxy|bind)?(?:[-_.\d]|$)", re.I)),
    ("C++", re.compile(r"(?:libstdc\+\+|libc\+\+|libc\+\+abi|msvcp\d*|concrt\d*)", re.I)),
)

WINDOWS_DYNAMIC_CRT_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"^ucrtbase(?:d)?\.dll$", re.I),
    re.compile(r"^vcruntime\d*(?:_\d+)?d?\.dll$", re.I),
    re.compile(r"^msvcr\d*d?\.dll$", re.I),
    re.compile(r"^msvcrt\.dll$", re.I),
    re.compile(r"^msvcp\d*d?\.dll$", re.I),
    re.compile(r"^concrt\d*d?\.dll$", re.I),
    re.compile(r"^api-ms-win-crt-.*\.dll$", re.I),
)

MACH_MAGICS = {
    b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
    b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca",
    b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca",
}


def parse_glibc_versions(text: str) -> list[tuple[int, int]]:
    return sorted({(int(a), int(b)) for a, b in re.findall(r"GLIBC_(\d+)\.(\d+)", text)})


def enforce_glibc_baseline(text: str) -> tuple[int, int] | None:
    versions = parse_glibc_versions(text)
    if not versions:
        return None
    newest = max(versions)
    if newest > MAX_GLIBC:
        raise RuntimeError(
            f"Linux plugin requires GLIBC_{newest[0]}.{newest[1]}, newer than "
            f"the supported GLIBC_{MAX_GLIBC[0]}.{MAX_GLIBC[1]} baseline"
        )
    return newest


def run(*args: str) -> str:
    try:
        completed = subprocess.run(
            args, check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT
        )
    except subprocess.CalledProcessError as error:
        output = error.stdout.strip() if isinstance(error.stdout, str) else ""
        detail = f": {output}" if output else ""
        raise RuntimeError(f"audit command failed ({' '.join(args)}){detail}") from error
    return completed.stdout


def require_tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"required audit tool not found: {name}")
    return path


def detect_binary_format(path: Path) -> str:
    with path.open("rb") as handle:
        prefix = handle.read(4)
    if prefix == b"\x7fELF":
        return "elf"
    if prefix[:2] == b"MZ":
        return "pe"
    if prefix in MACH_MAGICS:
        return "macho"
    raise RuntimeError(f"unrecognized native binary format (magic {prefix.hex()}): {path}")


def _header_fields(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            fields[key.strip().casefold()] = value.strip()
    return fields


def validate_elf_header(text: str) -> None:
    fields = _header_fields(text)
    expected = {
        "class": "ELF64",
        "data": "2's complement, little endian",
        "type": "DYN (Shared object file)",
        "machine": "Advanced Micro Devices X86-64",
    }
    for field, value in expected.items():
        if fields.get(field) != value:
            raise RuntimeError(
                f"Linux plugin has wrong ELF {field}: expected {value!r}, got {fields.get(field)!r}"
            )


def validate_pe_headers(text: str) -> None:
    if not re.search(r"(?im)^\s*8664 machine \(x64\)\s*$", text):
        raise RuntimeError("Windows plugin is not an x86-64 PE image")
    if not re.search(r"(?im)^\s*20B magic # \(PE32\+\)\s*$", text):
        raise RuntimeError("Windows plugin is not a 64-bit PE32+ image")
    if not re.search(r"(?im)^\s*[0-9A-F]+ characteristics\s*$", text) or not re.search(
        r"(?im)^\s*DLL\s*$", text
    ):
        raise RuntimeError("Windows image is not marked as a DLL")


def validate_macho_header(text: str, architecture: str) -> None:
    escaped = re.escape(architecture.upper())
    if not re.search(rf"(?im)^\s*MH_MAGIC_64\s+{escaped}\b.*\bBUNDLE\b", text):
        raise RuntimeError(
            f"macOS {architecture} slice is not a 64-bit Mach-O bundle of the expected architecture"
        )


def parse_elf_dependencies(text: str) -> list[str]:
    return re.findall(r"\(NEEDED\).*?Shared library:\s*\[([^]]+)\]", text)


def parse_otool_dependencies(text: str) -> list[str]:
    dependencies: list[str] = []
    for line in text.splitlines():
        match = re.match(r"^\s+(\S+)\s+\(compatibility version ", line)
        if match:
            dependencies.append(match.group(1))
    return dependencies


def parse_dumpbin_dependencies(text: str) -> list[str]:
    return [
        match.group(1)
        for line in text.splitlines()
        if (match := re.match(r"^\s*([^\s:]+\.dll)\s*$", line, re.I))
    ]


def _dependency_basename(dependency: str) -> str:
    return re.split(r"[/\\]", dependency.strip())[-1].casefold()


def reject_forbidden_dependencies(dependencies: list[str]) -> None:
    violations: list[str] = []
    for dependency in dependencies:
        basename = _dependency_basename(dependency)
        for family, pattern in FORBIDDEN_DEPENDENCY_PATTERNS:
            if pattern.search(basename):
                violations.append(f"{dependency} ({family})")
                break
    if violations:
        raise RuntimeError(
            "forbidden dynamic dependencies found: " + ", ".join(sorted(set(violations)))
        )


def enforce_windows_static_crt(dependencies: list[str]) -> None:
    dynamic = sorted({
        dependency
        for dependency in dependencies
        if any(
            pattern.fullmatch(_dependency_basename(dependency))
            for pattern in WINDOWS_DYNAMIC_CRT_PATTERNS
        )
    })
    if dynamic:
        raise RuntimeError("Windows plugin is not statically CRT-linked: " + ", ".join(dynamic))


def parse_elf_exports(text: str) -> list[str]:
    exports: list[str] = []
    for line in text.splitlines():
        fields = line.split()
        if len(fields) >= 2 and len(fields[1]) == 1 and fields[1] not in {"U", "u"}:
            exports.append(fields[0])
    return exports


def parse_macho_exports(text: str) -> list[str]:
    return [
        line.strip()
        for line in text.splitlines()
        if line.strip() and not line.rstrip().endswith(":")
    ]


def parse_dumpbin_exports(text: str) -> list[str]:
    exports: list[str] = []
    for line in text.splitlines():
        match = re.match(r"^\s*\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\S+)", line, re.I)
        if match:
            exports.append(match.group(1))
    return exports


def _canonical_export(symbol: str) -> str:
    unversioned = symbol.split("@", 1)[0]
    return unversioned[1:] if unversioned.startswith("_") else unversioned


def validate_exports(symbols: list[str], *, context: str) -> list[str]:
    canonical = {_canonical_export(symbol) for symbol in symbols}
    missing = REQUIRED_EXPORTS - canonical
    unexpected = canonical - REQUIRED_EXPORTS
    if missing:
        raise RuntimeError(f"{context}: required exported symbols missing: {', '.join(sorted(missing))}")
    if unexpected:
        raise RuntimeError(f"{context}: unexpected public exports: {', '.join(sorted(unexpected))}")
    permitted_raw = REQUIRED_EXPORTS | {f"_{name}" for name in REQUIRED_EXPORTS}
    invalid = sorted(symbol for symbol in symbols if symbol.split("@", 1)[0] not in permitted_raw)
    if invalid:
        raise RuntimeError(f"{context}: unsupported export decoration: {', '.join(invalid)}")
    return sorted(canonical)


def validate_linux_resolution(text: str) -> None:
    failures = [
        line.strip()
        for line in text.splitlines()
        if re.search(r"\bnot found\b|\bundefined symbol\s*:", line, re.I)
    ]
    if failures:
        raise RuntimeError(
            "Linux loader reports unresolved dependencies or symbols: " + "; ".join(failures)
        )


def validate_macho_undefineds(text: str, *, architecture: str) -> None:
    failures: list[str] = []
    for line in text.splitlines():
        lower = line.casefold()
        if "dynamically looked up" in lower:
            failures.append(line.strip())
        elif "(undefined)" in lower and "(from " not in lower:
            failures.append(line.strip())
    if failures:
        raise RuntimeError(
            f"macOS {architecture} slice has unresolved dynamic-lookup symbols: "
            + "; ".join(failures)
        )


def audit_elf(path: Path) -> dict[str, object]:
    readelf, nm, ldd = require_tool("readelf"), require_tool("nm"), require_tool("ldd")
    headers = run(readelf, "--file-header", str(path))
    validate_elf_header(headers)
    dynamic = run(readelf, "--dynamic", str(path))
    dependencies = parse_elf_dependencies(dynamic)
    reject_forbidden_dependencies(dependencies)
    export_text = run(nm, "--dynamic", "--defined-only", "--format=posix", str(path))
    exports = parse_elf_exports(export_text)
    validate_exports(exports, context="Linux plugin")
    versions = run(readelf, "--version-info", str(path))
    newest_glibc = enforce_glibc_baseline(versions)
    resolution = run(ldd, "-r", str(path))
    validate_linux_resolution(resolution)
    return {
        "platform": "linux", "target": "x86_64-unknown-linux-gnu",
        "headers": headers, "dependencies": dependencies, "exports": exports,
        "symbol_versions": versions, "newest_glibc": newest_glibc,
        "resolution": resolution, "unresolved_symbols": [],
    }


def audit_macho(path: Path) -> dict[str, object]:
    lipo, otool, nm = require_tool("lipo"), require_tool("otool"), require_tool("nm")
    architecture_text = run(lipo, "-archs", str(path))
    architectures = architecture_text.split()
    if len(architectures) != 2 or set(architectures) != MACOS_ARCHITECTURES:
        raise RuntimeError(
            "macOS plugin must contain exactly universal x86_64 and arm64 slices: "
            + architecture_text.strip()
        )

    headers: dict[str, str] = {}
    dependencies: dict[str, list[str]] = {}
    exports: dict[str, list[str]] = {}
    undefineds: dict[str, str] = {}
    for architecture in ("x86_64", "arm64"):
        header_text = run(otool, "-hv", "-arch", architecture, str(path))
        validate_macho_header(header_text, architecture)
        headers[architecture] = header_text
        dependency_text = run(otool, "-L", "-arch", architecture, str(path))
        parsed_dependencies = parse_otool_dependencies(dependency_text)
        reject_forbidden_dependencies(parsed_dependencies)
        dependencies[architecture] = parsed_dependencies
        export_text = run(nm, "-gjU", "-arch", architecture, str(path))
        parsed_exports = parse_macho_exports(export_text)
        validate_exports(parsed_exports, context=f"macOS {architecture} slice")
        exports[architecture] = parsed_exports
        undefined_text = run(nm, "-m", "-u", "-arch", architecture, str(path))
        validate_macho_undefineds(undefined_text, architecture=architecture)
        undefineds[architecture] = undefined_text

    return {
        "platform": "macos", "target": "universal-apple-darwin (x86_64, arm64)",
        "architectures": architectures, "headers": headers,
        "dependencies": dependencies, "exports": exports,
        "undefined_imports": undefineds, "unresolved_symbols": [],
    }


def audit_pe(path: Path) -> dict[str, object]:
    dumpbin = require_tool("dumpbin")
    headers = run(dumpbin, "/HEADERS", str(path))
    validate_pe_headers(headers)
    dependency_text = run(dumpbin, "/DEPENDENTS", str(path))
    dependencies = parse_dumpbin_dependencies(dependency_text)
    reject_forbidden_dependencies(dependencies)
    enforce_windows_static_crt(dependencies)
    export_text = run(dumpbin, "/EXPORTS", str(path))
    exports = parse_dumpbin_exports(export_text)
    validate_exports(exports, context="Windows plugin")
    imports = run(dumpbin, "/IMPORTS", str(path))
    return {
        "platform": "windows", "target": "x86_64-pc-windows-msvc",
        "headers": headers, "dependencies": dependencies, "exports": exports,
        "imports": imports, "unresolved_symbols": [],
        # PE/COFF links symbol references into the import table. The build's
        # immediately following target-native loader harness resolves the DLLs.
        "resolution_check": "PE import table structurally linked; target-native loader test required",
    }


def audit(path: Path) -> dict[str, object]:
    if not path.is_file() or path.stat().st_size == 0:
        raise RuntimeError(f"plugin does not exist or is empty: {path}")
    binary_format = detect_binary_format(path)
    platform_report = {"elf": audit_elf, "macho": audit_macho, "pe": audit_pe}[binary_format](path)
    report: dict[str, object] = {
        "path": str(path), "size": path.stat().st_size, "format": binary_format,
        **platform_report, "audit_passed": True,
        "loader_test_required_before_checksum": True,
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return report


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise SystemExit("usage: audit_binary.py PATH_TO_PLUGIN")
    audit(Path(argv[1]).resolve())
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
